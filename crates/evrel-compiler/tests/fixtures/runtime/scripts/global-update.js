evrelGlobalUpdateValue = 4;
var postfix = evrelGlobalUpdateValue--;
var prefix = --evrelGlobalUpdateValue;

__evrel.observe(
    "global update",
    postfix,
    prefix,
    evrelGlobalUpdateValue,
    globalThis.evrelGlobalUpdateValue,
);
delete globalThis.evrelGlobalUpdateValue;
