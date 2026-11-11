fn main() {
    println!("cargo:rerun-if-changed=src/grammar.lalrpop");
    lalrpop::process_src().expect("failed to generate the Scripture parser");
}
