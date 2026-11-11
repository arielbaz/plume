fn main() {
    if plume_lib::headless::maybe_run() {
        return;
    }
    plume_lib::run();
}
