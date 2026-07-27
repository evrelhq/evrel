const escaped = "A\x42\u0043\u{44}";
const separators = 1_000_000;

__evrel.observe(
    "numeric and string literals",
    0b101010,
    0o52,
    0x2a,
    42e0,
    .42e2,
    separators,
    0x2_an,
    escaped,
    "𝌆".length,
);
