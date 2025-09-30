mod syntax {
    pub struct SyntaxValidator;
    
    impl SyntaxValidator {
        pub fn new() -> Self {
            Self
        }
    }
}

use syntax::SyntaxValidator;

fn main() {
    let _validator = SyntaxValidator::new();
    println!("Test passed");
}