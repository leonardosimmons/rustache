#[derive(PartialEq, Eq)]
struct A {}

#[derive(PartialEq, Eq)]
struct B {}

pub trait AnyTrait {
    fn as_any(&self) -> &dyn std::any::Any;
}
impl Eq for dyn AnyTrait {}
impl PartialEq<Self> for dyn AnyTrait {
    fn eq(&self, other: &Self) -> bool {
        let x = self.as_any();
        let y = other.as_any();
        if x.is::<A>() && y.is::<A>() {
            true
        } else if x.is::<B>() && y.is::<B>() {
            true
        } else {
            false
        }
    }
}

pub trait Memoize<K, V>
    where
        K: Copy + Eq + std::hash::Hash,
        V: Copy,
{
    fn memoize(&mut self, args: K) -> V;
    fn value(&mut self, args: K) -> V;
}
