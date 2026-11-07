#[derive(Clone, Copy)]
enum Kind { Powder, Liquid, Smoke, Steam }
#[derive(Debug, PartialEq)]
enum Boundary { Stop, Remove, RemoveAndCreditHumidity }
fn at_margin(kind: Kind) -> Boundary {
    match kind {
        Kind::Powder | Kind::Liquid => Boundary::Stop,
        Kind::Smoke => Boundary::Remove,
        Kind::Steam => Boundary::RemoveAndCreditHumidity,
    }
}
#[test]
fn boundary_policy_is_material_aware() {
    assert_eq!(at_margin(Kind::Powder), Boundary::Stop);
    assert_eq!(at_margin(Kind::Liquid), Boundary::Stop);
    assert_eq!(at_margin(Kind::Smoke), Boundary::Remove);
    assert_eq!(at_margin(Kind::Steam), Boundary::RemoveAndCreditHumidity);
}
