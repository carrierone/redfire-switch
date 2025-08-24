use redfire_switch::g729_simple_test;

fn main() {
    println!("Running G.729 Simple Performance Test...\n");
    g729_simple_test::run_g729_performance_test();
    println!("\nTest completed successfully!");
}
