// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

#![forbid(unsafe_code)]
#![allow(dead_code)]

mod availability_checks;
mod data_notification;
mod data_stream;
mod error;
mod stream_progress_tracker;
mod streaming_client;
mod streaming_service;

#[cfg(test)]
mod tests;
