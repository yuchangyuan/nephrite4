pub mod conf;
pub mod proj;
pub mod util;
pub mod store;
pub mod error;
pub mod git;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
