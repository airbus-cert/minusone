console.log(b(2, 3));
function a(x, r) {
    if (r == 0) {
        return x;
    } else {
        return b(x * x, r - 1);
    }
}
function b(y, r) {
    if (r == 0) {
        return y;
    } else {
        return a(y * y, r - 1);
    }
}
