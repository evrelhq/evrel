class Base {}
class Derived extends Base {}

const inherited = { inherited: true };
const object = Object.create(inherited);
object.own = true;
const instance = new Derived();

__evrel.observe(
    "relational operators",
    instance instanceof Derived,
    instance instanceof Base,
    instance instanceof Object,
    "own" in object,
    "inherited" in object,
    "missing" in object,
);
