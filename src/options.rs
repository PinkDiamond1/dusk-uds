/// Set of options to define the behavior of the UDS listener
pub struct Options {
    /// Define the number of worker threads to listen
    pub workers: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            workers: num_cpus::get(),
        }
    }
}
