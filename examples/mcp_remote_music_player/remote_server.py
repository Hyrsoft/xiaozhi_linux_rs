#!/usr/bin/env python3
import socket
import json
import os
import subprocess
import threading
import sys

# 该脚本在另外一台负责播放音乐的设备（例如音响板子或电脑）上运行
# 监听本地指定的端口，等待接收来自主设备的控制指令

HOST = '0.0.0.0'
PORT = 8899

current_player = None
music_files_cache = []

def play_music_thread(music_name):
    global current_player
    print(f"准备播放: {music_name}")
    
    # 检查文件是否存在
    if not os.path.exists(music_name):
        print(f"错误：找不到文件 '{music_name}'")
        return

    # 先停止当前正在播放的
    if current_player is not None:
        try:
            current_player.terminate()
            current_player.wait()
        except:
            pass

    cmd = ["ffplay", "-nodisp", "-autoexit", "-loglevel", "quiet", music_name]
    try:
        current_player = subprocess.Popen(cmd)
        current_player.wait()
        print("播放结束")
    except Exception as e:
        print(f"播放出错: {e}")
    finally:
        current_player = None

def handle_client(conn, addr):
    print(f"收到来自 {addr} 的连接")
    try:
        # rust网关发送的 JSON 参数后面带有一个换行符
        fp = conn.makefile('r', encoding='utf-8')
        line = fp.readline()
        if not line:
            return
            
        params = json.loads(line)
        print(f"收到参数: {params}")
        
        # 只要接收到命令，不论是什么参数，直接播放硬编码音乐
        music_name = "/home/hao/原色.mp3"
        
        t = threading.Thread(target=play_music_thread, args=(music_name,))
        t.start()
        
        msg = f"成功：远端设备已开始播放音乐 {music_name}"
        conn.sendall(msg.encode('utf-8'))
                
    except json.JSONDecodeError:
        conn.sendall("错误：接收到的参数不是合法的 JSON".encode('utf-8'))
    except Exception as e:
        print(f"连接处理异常: {e}")
        try:
            conn.sendall(f"服务器内部异常: {str(e)}".encode('utf-8'))
        except:
            pass
    finally:
        conn.close()

def main():
    print(f"启动远端音乐播放服务，监听 {HOST}:{PORT}")
    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    
    try:
        server.bind((HOST, PORT))
        server.listen(5)
        
        while True:
            conn, addr = server.accept()
            # 为每个连接启动新线程处理
            t = threading.Thread(target=handle_client, args=(conn, addr))
            t.start()
            
    except KeyboardInterrupt:
        print("\n服务已退出")
    finally:
        server.close()

if __name__ == "__main__":
    main()
