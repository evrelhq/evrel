function Base() {
    this.target = new.target;
}

function Derived() {
    return Reflect.construct(Base, [], new.target);
}

Object.setPrototypeOf(Derived.prototype, Base.prototype);
const direct = new Base();
const derived = new Derived();

__evrel.observe(
    "new target",
    direct.target === Base,
    derived.target === Derived,
    derived instanceof Derived,
    derived instanceof Base,
);
