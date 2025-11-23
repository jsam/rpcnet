pub mod generated {
    pub mod inference {
        pub mod types;
        pub mod server;
        pub mod client;
        
        pub use types::*;
        pub use server::*;
        pub use client::*;
    }
    
    pub mod directorregistry {
        pub mod types;
        pub mod server;
        pub mod client;
        
        pub use types::*;
        pub use server::*;
        pub use client::*;
    }
}
