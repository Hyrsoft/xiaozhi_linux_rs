#[cfg(windows)]
use process_wrap::tokio::JobObject;
#[cfg(unix)]
use process_wrap::tokio::ProcessGroup;
use process_wrap::tokio::{KillOnDrop, TokioChildWrapper, TokioCommandWrap};
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub const MAX_PROCESS_OUTPUT: usize = 1024 * 1024;

#[derive(Clone, Debug)]
pub struct ProcessRequest {
    pub executable: String,
    pub args: Vec<String>,
    pub stdin: Vec<u8>,
    pub timeout: Duration,
}

#[derive(Debug)]
pub struct ProcessResult {
    pub status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub stdout_truncated: bool,
    pub stderr_truncated: bool,
}

#[derive(Clone, Default)]
pub struct ProcessRunner;

impl ProcessRunner {
    pub async fn run(
        &self,
        request: ProcessRequest,
        cancellation: CancellationToken,
    ) -> Result<ProcessResult, String> {
        let executable = request.executable.clone();
        let mut command = tokio::process::Command::new(&request.executable);
        command
            .args(&request.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let mut command = TokioCommandWrap::from(command);
        command.wrap(KillOnDrop);
        #[cfg(unix)]
        command.wrap(ProcessGroup::leader());
        #[cfg(windows)]
        command.wrap(JobObject);

        let mut child = command
            .spawn()
            .map_err(|error| format!("Failed to spawn {executable}: {error}"))?;

        let mut stdin = child.stdin().take();
        let input = request.stdin;
        let stdin_task = tokio::spawn(async move {
            if let Some(mut stdin) = stdin.take() {
                stdin.write_all(&input).await?;
                stdin.shutdown().await?;
            }
            Ok::<_, std::io::Error>(())
        });

        let stdout = child
            .stdout()
            .take()
            .ok_or_else(|| format!("Failed to capture stdout for {executable}"))?;
        let stderr = child
            .stderr()
            .take()
            .ok_or_else(|| format!("Failed to capture stderr for {executable}"))?;
        let stdout_task = tokio::spawn(read_capped(stdout));
        let stderr_task = tokio::spawn(read_capped(stderr));

        let wait_result = tokio::select! {
            result = Box::into_pin(child.wait()) => result.map_err(|error| format!("Failed to wait for {executable}: {error}")),
            _ = tokio::time::sleep(request.timeout) => {
                terminate(&mut child).await;
                Err(format!("Process {executable} timed out after {} ms", request.timeout.as_millis()))
            }
            _ = cancellation.cancelled() => {
                terminate(&mut child).await;
                Err(format!("Process {executable} was cancelled"))
            }
        };

        stdin_task.abort();
        let stdout = join_reader(stdout_task, "stdout").await?;
        let stderr = join_reader(stderr_task, "stderr").await?;
        let status = wait_result?;

        if stdout.1 || stderr.1 {
            return Err(format!(
                "Process {executable} output exceeded the {} byte limit (stdout truncated: {}, stderr truncated: {})",
                MAX_PROCESS_OUTPUT, stdout.1, stderr.1
            ));
        }

        Ok(ProcessResult {
            status,
            stdout: stdout.0,
            stderr: stderr.0,
            stdout_truncated: stdout.1,
            stderr_truncated: stderr.1,
        })
    }
}

async fn terminate(child: &mut Box<dyn TokioChildWrapper>) {
    if let Err(error) = Box::into_pin(child.kill()).await {
        log::warn!("Failed to terminate subprocess group: {error}");
    }
}

async fn read_capped(mut reader: impl AsyncRead + Unpin) -> std::io::Result<(Vec<u8>, bool)> {
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut truncated = false;
    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = MAX_PROCESS_OUTPUT.saturating_sub(output.len());
        let retained = remaining.min(read);
        output.extend_from_slice(&buffer[..retained]);
        truncated |= retained < read;
    }
    Ok((output, truncated))
}

async fn join_reader(
    task: JoinHandle<std::io::Result<(Vec<u8>, bool)>>,
    stream: &str,
) -> Result<(Vec<u8>, bool), String> {
    task.await
        .map_err(|error| format!("Failed to join {stream} reader: {error}"))?
        .map_err(|error| format!("Failed to read process {stream}: {error}"))
}

#[derive(Clone)]
pub struct ProcessSupervisor {
    runner: ProcessRunner,
    cancellation: CancellationToken,
    tasks: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl Default for ProcessSupervisor {
    fn default() -> Self {
        Self {
            runner: ProcessRunner,
            cancellation: CancellationToken::new(),
            tasks: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl ProcessSupervisor {
    pub async fn run(&self, request: ProcessRequest) -> Result<ProcessResult, String> {
        self.runner
            .run(request, self.cancellation.child_token())
            .await
    }

    pub fn spawn<F>(&self, future: F)
    where
        F: std::future::Future<Output = ()> + Send + 'static,
    {
        let handle = tokio::spawn(future);
        self.tasks
            .lock()
            .expect("process task mutex poisoned")
            .push(handle);
    }

    pub async fn shutdown(&self) {
        self.cancellation.cancel();
        let tasks = {
            let mut tasks = self.tasks.lock().expect("process task mutex poisoned");
            std::mem::take(&mut *tasks)
        };
        for task in tasks {
            let _ = task.await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn shell(script: &str, timeout: Duration) -> ProcessRequest {
        ProcessRequest {
            executable: "sh".into(),
            args: vec!["-c".into(), script.into()],
            stdin: Vec::new(),
            timeout,
        }
    }

    #[tokio::test]
    async fn captures_success_and_nonzero_exit() {
        let runner = ProcessRunner;
        let result = runner
            .run(
                shell(
                    "printf output; printf error >&2; exit 7",
                    Duration::from_secs(1),
                ),
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(result.status.code(), Some(7));
        assert_eq!(result.stdout, b"output");
        assert_eq!(result.stderr, b"error");
    }

    #[tokio::test]
    async fn enforces_timeout_and_cancellation() {
        let runner = ProcessRunner;
        let timed_out = runner
            .run(
                shell("sleep 10", Duration::from_millis(20)),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(timed_out.contains("timed out"));

        let token = CancellationToken::new();
        token.cancel();
        let cancelled = runner
            .run(shell("sleep 10", Duration::from_secs(10)), token)
            .await
            .unwrap_err();
        assert!(cancelled.contains("cancelled"));
    }

    #[tokio::test]
    async fn rejects_output_over_limit() {
        let error = ProcessRunner
            .run(
                shell("yes x | head -c 1048577", Duration::from_secs(2)),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.contains("output exceeded"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_kills_descendant_processes() {
        let pid_file = std::env::temp_dir().join(format!(
            "xiaozhi-process-group-{}-{}.pid",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let script = format!("sleep 10 & echo $! > '{}'; wait", pid_file.display());
        let error = ProcessRunner
            .run(
                shell(&script, Duration::from_millis(50)),
                CancellationToken::new(),
            )
            .await
            .unwrap_err();
        assert!(error.contains("timed out"));

        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let status = tokio::process::Command::new("kill")
            .args(["-0", pid.trim()])
            .status()
            .await
            .unwrap();
        let _ = std::fs::remove_file(pid_file);
        assert!(!status.success(), "descendant process survived group kill");
    }

    #[tokio::test]
    async fn supervisor_shutdown_cleans_background_tasks() {
        let supervisor = ProcessSupervisor::default();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_after_run = completed.clone();
        let background_supervisor = supervisor.clone();
        supervisor.spawn(async move {
            let result = background_supervisor
                .run(shell("sleep 10", Duration::from_secs(10)))
                .await;
            assert!(result.unwrap_err().contains("cancelled"));
            completed_after_run.store(true, Ordering::SeqCst);
        });

        tokio::task::yield_now().await;
        supervisor.shutdown().await;
        assert!(completed.load(Ordering::SeqCst));
    }
}
