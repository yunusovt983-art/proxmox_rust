//! Contract test runner binary

use clap::{Arg, Command};
use pve_network_test::contract_tests::ContractTester;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::init();

    let matches = Command::new("contract-test")
        .about("Run contract tests comparing Rust and Perl API responses")
        .arg(
            Arg::new("node")
                .short('n')
                .long("node")
                .value_name("NODE")
                .help("Node name to test")
                .default_value("localhost"),
        )
        .arg(
            Arg::new("perl-url")
                .short('u')
                .long("perl-url")
                .value_name("URL")
                .help("Perl API base URL")
                .default_value("http://localhost:8006"),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Output report to file"),
        )
        .arg(
            Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Verbose output")
                .action(clap::ArgAction::SetTrue),
        )
        .get_matches();

    let node = matches.get_one::<String>("node").unwrap();
    let perl_url = matches.get_one::<String>("perl-url").unwrap();
    let output_file = matches.get_one::<String>("output");
    let verbose = matches.get_flag("verbose");

    println!("Running contract tests for node: {}", node);
    println!("Perl API URL: {}", perl_url);
    println!();

    let tester = ContractTester::with_perl_url(node, perl_url);
    let results = tester.run_all_tests().await;

    if verbose {
        results.print_summary();
    } else {
        println!(
            "Contract Test Results: {}/{} passed ({:.1}%)",
            results.passed_tests,
            results.total_tests,
            (results.passed_tests as f64 / results.total_tests as f64) * 100.0
        );
    }

    if let Some(output_path) = output_file {
        let report = results.generate_report();
        std::fs::write(output_path, report)?;
        println!("Report written to: {}", output_path);
    }

    // Exit with error code if tests failed
    if results.failed_tests > 0 {
        std::process::exit(1);
    }

    Ok(())
}
