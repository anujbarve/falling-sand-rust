struct Gate { name: &'static str, passed: bool }
fn ready(gates:&[Gate])->Result<(),Vec<&'static str>> {
    let failures:Vec<_>=gates.iter().filter(|g|!g.passed).map(|g|g.name).collect();
    if failures.is_empty() { Ok(()) } else { Err(failures) }
}
fn main() {
    let gates=[Gate{name:"headless regression",passed:true},
        Gate{name:"floor-machine frame times",passed:false},
        Gate{name:"native package smoke tests",passed:true},
        Gate{name:"complete-save recovery",passed:true}];
    match ready(&gates) {
        Ok(())=>println!("candidate ready for explicit publication review"),
        Err(missing)=>println!("release blocked: {}",missing.join(", ")),
    }
}
