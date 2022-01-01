pub trait CacheCapacityController<K, V>
    where
        K: Eq + std::hash::Hash,
{
    fn capacity(self, entries: usize) -> Self;
    fn check_capacity(&self) -> Result<(), ()>;
    fn clean_up(&mut self) -> Result<(K, V), ()>;
}

pub trait CacheExpirationController {
    fn expires(self, seconds: u64) -> Self;
    fn revalidate(self, status: bool) -> Self;
    fn validate_expiration(&self) -> Result<(), &str>;
}
