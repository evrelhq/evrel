function tag(strings) {
    return [strings[0], strings.raw[0]];
}

const result = tag`\unicode`;
__evrel.observe("invalid tagged escape", result[0], result[1]);
