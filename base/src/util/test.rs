pub struct MethodMock<Args, Ret> {
    pub args: Vec<Args>,
    pub rets: Vec<Ret>,
}

impl<Args, Ret> MethodMock<Args, Ret> {
    pub fn new() -> Self {
        MethodMock {
            args: vec![],
            rets: vec![],
        }
    }

    pub fn call(&mut self, args: Args) -> Ret {
        assert!(!self.rets.is_empty());
        self.args.push(args);
        self.rets.pop().unwrap()
    }
}

impl<Args, Ret> Drop for MethodMock<Args, Ret> {
    fn drop(&mut self) {
        assert!(self.args.is_empty());
        assert!(self.rets.is_empty());
    }
}
