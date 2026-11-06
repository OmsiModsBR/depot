use openomsi_game::testing::quit::*;

#[test]
fn a_signal_wakes_the_loop_once() {
    let (tx, rx) = std::sync::mpsc::channel();
    install(move |sig| tx.send(sig).unwrap());
    assert!(requested().is_none());
    // SAFETY: raising a handled signal in our own process
    unsafe { sys::raise(SIGTERM) };
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_secs(5)).unwrap(),
        SIGTERM
    );
    assert_eq!(requested(), Some(SIGTERM));
    // woken once: the watcher is done (and has dropped its sender)
    assert_eq!(
        rx.recv_timeout(std::time::Duration::from_millis(300)),
        Err(std::sync::mpsc::RecvTimeoutError::Disconnected)
    );
    // (a second signal would end the test process: not raised here)
}
