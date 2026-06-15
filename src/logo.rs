//! The termem wordmark: "ter" in teal, "mem" in violet. Built from a small
//! block font so the two halves can be colored independently.

pub const TER_RGB: (u8, u8, u8) = (94, 234, 212); // teal
pub const MEM_RGB: (u8, u8, u8) = (192, 132, 252); // violet

const T: [&str; 5] = ["█████", "  █  ", "  █  ", "  █  ", "  █  "];
const E: [&str; 5] = ["█████", "█    ", "████ ", "█    ", "█████"];
const R: [&str; 5] = ["████ ", "█   █", "████ ", "█  █ ", "█   █"];
const M: [&str; 5] = ["█   █", "██ ██", "█ █ █", "█   █", "█   █"];

/// The five rows of the "ter" half.
pub fn ter_rows() -> [String; 5] {
    std::array::from_fn(|i| format!("{} {} {}", T[i], E[i], R[i]))
}

/// The five rows of the "mem" half.
pub fn mem_rows() -> [String; 5] {
    std::array::from_fn(|i| format!("{} {} {}", M[i], E[i], M[i]))
}

/// ANSI-colored banner for non-interactive output. `pad` left-indents each row.
pub fn ansi_banner(pad: usize) -> String {
    let (tr, tg, tb) = TER_RGB;
    let (mr, mg, mb) = MEM_RGB;
    let ter = ter_rows();
    let mem = mem_rows();
    let indent = " ".repeat(pad);
    let mut out = String::new();
    for i in 0..5 {
        out.push_str(&format!(
            "{indent}\x1b[1;38;2;{tr};{tg};{tb}m{}\x1b[0m  \x1b[1;38;2;{mr};{mg};{mb}m{}\x1b[0m\n",
            ter[i], mem[i]
        ));
    }
    out
}
