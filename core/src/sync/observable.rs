use arc_swap::ArcSwap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::watch;
use typed_builder::TypedBuilder;

#[derive(TypedBuilder)]
struct State<T: PartialEq + Send + Sync> {
    arc: ArcSwap<T>,
    #[builder(default, setter(skip))]
    version: AtomicU64,
    tx: watch::Sender<u64>,
}

#[derive(Clone)]
pub struct Observable<T: PartialEq + Send + Sync>(Arc<State<T>>);

impl<T: PartialEq + Send + Sync> Observable<T> {
    pub fn new(initial: T) -> (Observable<T>, Subscription<T>) {
        let (tx, _) = watch::channel(0);
        let state = State::builder()
            .arc(ArcSwap::from_pointee(initial))
            .tx(tx)
            .build();
        let observable = Self(Arc::new(state));
        let subscription = observable.subscribe();
        (observable, subscription)
    }

    fn state(&self) -> Arc<State<T>> {
        self.0.clone()
    }

    /// Subscribe to change notifications
    pub fn subscribe(&self) -> Subscription<T> {
        let state = self.state();
        let rx = state.tx.subscribe();

        Subscription::builder().state(state).rx(rx).build()
    }

    /// Apply an update function *optimistically* and retry if stale.
    ///
    /// If `f` produces the same value (PartialEq), no update or notify occurs.
    pub fn update<F>(&self, mut f: F)
    where
        F: FnMut(&T) -> T,
    {
        let state = self.state();
        loop {
            let current = state.arc.load_full(); // snapshot Arc<T>
            let new_value = f(&current); // derive candidate next state

            if *current == new_value {
                break; // no observable change → skip notify
            }

            let new_arc = Arc::new(new_value);
            let prev = state.arc.compare_and_swap(&current, new_arc.clone());

            if Arc::ptr_eq(&prev, &current) {
                // Successful commit → bump version + notify
                let v = state.version.fetch_add(1, Ordering::SeqCst) + 1;
                let _ = state.tx.send(v);
                break;
            }

            // Else: someone else updated concurrently → retry
        }
    }
}

impl<T: PartialEq + Send + Sync + Default> Default for Observable<T> {
    fn default() -> Self {
        let (observable, _) = Self::new(T::default());
        observable
    }
}

#[derive(TypedBuilder, Clone)]
pub struct Subscription<T: PartialEq + Send + Sync> {
    state: Arc<State<T>>,
    rx: watch::Receiver<u64>,
}

impl<T: PartialEq + Send + Sync> Subscription<T> {
    pub fn current(&self) -> Arc<T> {
        self.state.arc.load_full()
    }

    pub async fn changed(&mut self) -> Arc<T> {
        let _ = self.rx.changed().await;
        self.current()
    }
}
