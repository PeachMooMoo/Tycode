// Database module for Aurora DSQL interactions
mod audit;
pub mod dashboard;
mod dsql;
mod memory;

// Re-export the main DSQL client
pub use dsql::DsqlClient;
