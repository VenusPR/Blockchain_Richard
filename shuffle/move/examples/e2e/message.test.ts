// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0
//
// This file is generated on new project creation.

// deno-lint-ignore-file no-explicit-any
import { assert, assertEquals } from "https://deno.land/std@0.85.0/testing/asserts.ts";
import * as MessageHelper from "../main/txn_builders/helper.ts";
import * as Shuffle from "../repl.ts";
import * as utils from "./utils.ts";

Shuffle.test("Test Assert", () => {
  assert("Hello");
});

Shuffle.test("Ability to set message", async () => {
  assert(utils.deployMessageModule()); // Full Disclosure: Faked
  assert(utils.setMessage(Shuffle.senderAddress, "hello diem core eng")); // Full Disclosure: Faked, needs to be async

  const sender = Shuffle.senderAddress;
  const resources = await Shuffle.resources(sender);
  const messageResource = MessageHelper.messagesFrom(resources)[0];

  assertEquals(
    MessageHelper.hex2a(messageResource["value"]["message"]),
    "hello diem core eng",
  );
});
