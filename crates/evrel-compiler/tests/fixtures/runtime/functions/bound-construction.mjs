function Constructor(first, second) {
    this.total = first + second;
}
const receiver = { ignored: true };
const Bound = Constructor.bind(receiver, 20);
const instance = new Bound(22);

__evrel.observe(
    "bound construction",
    instance.total,
    instance instanceof Constructor,
    instance instanceof Bound,
    receiver.total,
    Bound.length,
);
