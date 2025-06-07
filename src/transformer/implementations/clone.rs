use crate::Transformer;

impl<I, O, E> Clone for Transformer<I, O, E> {
    fn clone(&self) -> Self {
        Self {
            transform: self.transform.clone(),
        }
    }
}
