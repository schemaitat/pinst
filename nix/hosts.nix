{
  andre-linux = { system = "x86_64-linux"; username = "andre"; homeDirectory = "/home/andre"; };
  andre-linux-arm64 = { system = "aarch64-linux"; username = "andre"; homeDirectory = "/home/andre"; };
  andre-macos = { system = "aarch64-darwin"; username = "andre"; homeDirectory = "/Users/andre"; };
  andre-macos-intel = { system = "x86_64-darwin"; username = "andre"; homeDirectory = "/Users/andre"; };
  # Only used in disposable containers/CI. Never activated on the live account.
  test-linux = { system = "x86_64-linux"; username = "root"; homeDirectory = "/tmp/pinst-home"; };
  test-linux-next = { system = "x86_64-linux"; username = "root"; homeDirectory = "/tmp/pinst-home"; sessionVariables.PINST_TEST_GENERATION = "next"; };
  test-macos = { system = "aarch64-darwin"; username = "runner"; homeDirectory = "/tmp/pinst-home"; };
  test-macos-next = { system = "aarch64-darwin"; username = "runner"; homeDirectory = "/tmp/pinst-home"; sessionVariables.PINST_TEST_GENERATION = "next"; };
  test-macos-intel = { system = "x86_64-darwin"; username = "runner"; homeDirectory = "/tmp/pinst-home"; };
  test-macos-intel-next = { system = "x86_64-darwin"; username = "runner"; homeDirectory = "/tmp/pinst-home"; sessionVariables.PINST_TEST_GENERATION = "next"; };
}
