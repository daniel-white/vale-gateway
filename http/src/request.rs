pub trait RequestMatchContext {
    fn path_prefix(&self) -> Option<&str>;
}
