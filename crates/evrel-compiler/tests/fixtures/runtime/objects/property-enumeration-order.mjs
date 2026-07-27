const symbol = Symbol("symbol");
const object = {};
object.second = 2;
object[10] = "ten";
object[2] = "two";
object.first = 1;
object[symbol] = 3;

__evrel.observe(
    "property order",
    Object.keys(object).join(","),
    Reflect.ownKeys(object).slice(0, 4).join(","),
    Reflect.ownKeys(object)[4] === symbol,
);
