function removePendingEntry(root, entry) {
  var pending = root.pending;
  pending !== null && pending.delete(entry);
}

const entries = new Set(["first", "second"]);
removePendingEntry({ pending: entries }, "first");
removePendingEntry({ pending: null }, "second");
console.log([...entries]);
