let count = 0;

for (; count < 3; ) {
    count++;
    break;
}

__evrel.observe("for break reconstruction", count);
