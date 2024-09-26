fn main() {
    let source = String::from(r#"test oskour paintest"#);
    println!("{:#?}", yarrow_core::tokenizer::tokenize(source));
}
