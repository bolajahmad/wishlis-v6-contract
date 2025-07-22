### Simple Wishlist app (NOT PRODUCTION READY)

## This contract is only for demo purposes and must not be relied on in production env.

### How it works?

This is a gamified wishlist implementation. A User, created a Wishlist target (to save up a certain **target** of token). With every fund_wish call, the User can increased their raised amount. If after a set timestamp, the raised amount is greater than or equal to the target, the User can claim their raised amount. 
Other Users, Contributors, can also fund_wish for the User as a challenge, if after the timestamp, the User's raised is less than the target, the Contributors share the total raised on the Wish.

### How to run

This contract should compile as is, if you have pop installed, run

```
    pop build --release
```

To run the unit tests,
``` 
    pop test
```