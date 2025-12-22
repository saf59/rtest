/*
use chrono::WeekdaySet;
use date_time_parser::Recognizable;
use event_parser::{pretty_print, to_event};
pub fn main() {
    parse_date_events("Show my objects changes during prev two weeks");
    parse_date_events("two weeks ago");
    parse_date_events("last two days");
}

fn parse_date_events(prompt: &str) {
    let event = to_event(prompt);
    let date_expr: Option<WeekdaySet>> = Recognizable::recognize(prompt);
    println!("{:#?}", date_expr);
    pretty_print(event);
}
*/
