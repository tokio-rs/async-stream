use std::marker::PhantomData;

pub struct Sender<T> {
    _p: PhantomData<T>,
}

pub struct Receiver<T> {
    _p: PhantomData<T>,
}

pub fn pair<T>() -> (Sender<T>, Receiver<T>) {
    unimplemented!();
}

impl<T> Sender<T> {
    pub async fn send(&mut self, value: T) {
        unimplemented!();
    }
}
