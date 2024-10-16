use tokio::sync::Mutex;

pub fn dead_mans_switch() -> (DeadMansSwitch, DeadMansSwitchObserver) {
    let (_sender, receiver) = tokio::sync::mpsc::channel::<()>(1);
    (
        DeadMansSwitch { _sender },
        DeadMansSwitchObserver(Mutex::new(receiver)),
    )
}

/// Allws to block until all workers (holders of DeadMansSwitch) are dead.
pub struct DeadMansSwitchObserver(Mutex<tokio::sync::mpsc::Receiver<()>>);

impl DeadMansSwitchObserver {
    pub fn wait_all_dead(&self) {
        // Will unblock when all senders are dropped. We keep one sender copy in each thread to join.
        while let Some(_) = self.0.blocking_lock().blocking_recv() {}
    }
}

/// Thingy just meant to be hold by the worker. When dropped, indicates worker is dead.
#[derive(Clone)]
pub struct DeadMansSwitch {
    _sender: tokio::sync::mpsc::Sender<()>,
}
