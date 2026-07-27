function updateAlternate(current, pendingProps) {
  var workInProgress = current.alternate;

  null === workInProgress
    ? ((workInProgress = { type: current.type }),
      (workInProgress.alternate = current),
      (current.alternate = workInProgress))
    : ((workInProgress.pendingProps = pendingProps),
      (workInProgress.type = current.type));

  workInProgress.flags = current.flags;
  return workInProgress;
}

const existing = { type: "old" };
const withAlternate = { alternate: existing, type: "new", flags: 1 };
const reused = updateAlternate(withAlternate, "pending");

__evrel.observe(
  "reuses existing alternate",
  reused === existing,
  reused.pendingProps,
  reused.type,
  reused.flags
);

const withoutAlternate = { alternate: null, type: "fresh", flags: 2 };
const created = updateAlternate(withoutAlternate, "ignored");

__evrel.observe(
  "creates missing alternate",
  created === withoutAlternate.alternate,
  created.alternate === withoutAlternate,
  created.type,
  created.flags
);
