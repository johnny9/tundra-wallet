"use strict";
// Public-only test fixtures. Never fund these descriptors.
const DESCRIPTOR_EXAMPLES = {
  "single": {
    "name": "Everyday (test)",
    "network": "signet",
    "descriptor": "wpkh([a1b2c3d4/84h/1h/0h]tpubDC9hiPWaCo6CNm48mwC6qut1oY9Hb4P9562p4yzYTChc4K8RXnyH9FTo2XyeyZS6giQQaKxh9PH83nmdVVTzawa2cKS9Afp8MjFp7M8S5B2/<0;1>/*)#2qugx73x"
  },
  "multi": {
    "name": "Savings (test)",
    "network": "signet",
    "descriptor": "wsh(sortedmulti(2,[a1b2c3d4/48h/1h/0h/2h]tpubDE2qaB2bR6mCKdLYsrB3B7nHVGBHmaB324fzRuUCgr1FfNX3FXSwgYTnLaxfgtsQuDbkJcHSENkmpUB4HgUt2wyNupDhmLhRraTjBNWDyTH/<0;1>/*,[1122aabb/48h/1h/0h/2h]tpubDE2qaB2bR6mCHU9T9Y9MSCpYEfQrLpqRXZg6ngzp29mvfwRytXMB8NLY8hGiZED38XThdAwgdhusYPiiMTwhQeNwYQoStyDYMjbLn3mY3Nj/<0;1>/*,[9988ccdd/48h/1h/0h/2h]tpubDE2qaB2bR6mCHpL2KmtcuJaeZ8YjRGKYfi65x8eA2ymfs2tvS46rsuzz5zoJku6nFfvMPG5uLWyoMKwSvbe9WA4vjYhEHsLyotiwLYFF1UN/<0;1>/*))#xcf2nvak"
  }
};
