#[derive(Clone, Copy)]
enum Source { Player, Nature }
#[derive(Clone, Copy)]
enum Command { Rain { column: i32 } }
fn accept(command: Command, source: Source, now: u64,
          ready: &mut u64, applied: &mut Vec<i32>) -> bool {
    if matches!(source,Source::Player) && now<*ready { return false; }
    match command { Command::Rain { column } => applied.push(column) }
    if matches!(source,Source::Player) { *ready=now.saturating_add(24_000); }
    true
}
fn main() {
    let (mut ready,mut applied)=(0,Vec::new());
    assert!(accept(Command::Rain{column:2},Source::Player,0,&mut ready,&mut applied));
    assert!(!accept(Command::Rain{column:2},Source::Player,1,&mut ready,&mut applied));
    assert!(accept(Command::Rain{column:3},Source::Nature,1,&mut ready,&mut applied));
    assert_eq!(applied,vec![2,3]);
    println!("shared effect path; distinct scheduling policies");
}
