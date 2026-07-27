const consonants = /[\p{ASCII}&&\P{Lowercase_Letter}]/v;
const difference = /[[a-z]--[aeiou]]/v;

__evrel.observe(
    "regexp unicode sets",
    consonants.test("A"),
    consonants.test("a"),
    difference.test("b"),
    difference.test("a"),
    consonants.flags,
);
