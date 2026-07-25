static const volatile char xiaozhi_auxval_stub_marker[] =
    "xiaozhi_auxval_stub_v1";

unsigned long getauxval(unsigned long type) {
    (void)type;
    (void)xiaozhi_auxval_stub_marker[0];
    return 0;
}
