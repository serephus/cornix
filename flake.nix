{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixit = {
      url = "github:serephus/nixit";
      inputs.nixpkgs.follows = "nixpkgs";
      inputs.flake-utils.follows = "flake-utils";
      inputs.rust-overlay.follows = "rust-overlay";
      inputs.naersk.follows = "naersk";
    };
  };

  outputs =
    {
      nixpkgs,
      flake-utils,
      nixit,
      rust-overlay,
      naersk,
      ...
    }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
    in
    {
      githubRepositories.cornix = nixit.lib.githubRepository {
        owner = "serephus";
        name = "cornix";

        description = "Corne v4 keyboard firmware";
        homepage = "https://github.com/serephus/cornix";
        topics = [
          "keyboard"
          "corne"
          "rp2040"
          "rust"
          "embassy"
        ];
        visibility = "public";

        features = {
          wiki.enable = false;
          issues.enable = true;
          projects.enable = false;
          discussions.enable = false;
        };

        is_template = false;
        is_archived = false;

        pull = {
          merge.enable = true;
          squash.enable = false;
          rebase.enable = false;
          auto_merge = true;
          delete_branch_on_merge = true;
          update_branch = true;
        };

        actions = {
          enable = true;
          policy = "all";
          default_token_permissions = "read";
          allow_pr_approval = false;
        };

        rulesets = {
          default = {
            enforcement = "active";
            conditions.ref_name.include = [ "~DEFAULT_BRANCH" ];
            rules = [
              { type = "deletion"; }
              { type = "non_fast_forward"; }
            ];
          };
          pr = {
            enforcement = "active";
            conditions.ref_name.include = [ "~DEFAULT_BRANCH" ];
            rules = [
              {
                type = "required_status_checks";
                parameters = {
                  do_not_enforce_on_create = true;
                  strict_required_status_checks_policy = true;
                  required_status_checks = [
                    { "context" = "ubuntu-latest-thumbv6m-none-eabi-stable"; }
                  ];
                };
              }
            ];
          };
        };
      };
    }
    // flake-utils.lib.eachSystem systems (
      system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        rust = pkgs.rust-bin.stable.latest.default.override {
          extensions = [
            "rust-src"
            "rustfmt"
            "clippy"
            "rust-analyzer"
          ];
          targets = [ "thumbv6m-none-eabi" ];
        };
        mkBuildApp =
          {
            name,
            extra ? "",
          }:
          pkgs.writeShellApplication {
            inherit name;
            runtimeInputs = [ rust ];
            text = ''
              if [ ! -f Cargo.toml ] || [ ! -f src/keymap.rs ]; then
                echo "run this from the cornix repository root" >&2
                exit 1
              fi
              cargo build --release -j4 ${extra}
            '';
          };
        mkUf2App =
          {
            name,
            extra ? "",
            suffix ? "",
          }:
          pkgs.writeShellApplication {
            inherit name;
            runtimeInputs = [
              rust
              pkgs.elf2uf2-rs
            ];
            text = ''
              if [ ! -f Cargo.toml ] || [ ! -f src/keymap.rs ]; then
                echo "run this from the cornix repository root" >&2
                exit 1
              fi
              cargo build --release -j4 ${extra}
              elf2uf2-rs target/thumbv6m-none-eabi/release/cornix target/thumbv6m-none-eabi/release/cornix${suffix}.uf2
            '';
          };
        buildV41App = mkBuildApp { name = "cornix-build-v41"; };
        uf2V41App = mkUf2App {
          name = "cornix-uf2-v41";
          suffix = "-v41";
        };
        buildV40App = mkBuildApp {
          name = "cornix-build-v40";
          extra = "--features v4_0";
        };
        uf2V40App = mkUf2App {
          name = "cornix-uf2-v40";
          extra = "--features v4_0";
          suffix = "-v40";
        };
      in
      {
        apps = {
          default = {
            type = "app";
            program = "${uf2V41App}/bin/cornix-uf2-v41";
            meta.description = "Build the unified v4.1 firmware and UF2";
          };
          uf2 = {
            type = "app";
            program = "${uf2V41App}/bin/cornix-uf2-v41";
            meta.description = "Build the unified v4.1 firmware and UF2";
          };
          build = {
            type = "app";
            program = "${buildV41App}/bin/cornix-build-v41";
            meta.description = "Build the unified v4.1 firmware";
          };
          uf2-v41 = {
            type = "app";
            program = "${uf2V41App}/bin/cornix-uf2-v41";
            meta.description = "Build the unified v4.1 half-duplex firmware and UF2";
          };
          build-v41 = {
            type = "app";
            program = "${buildV41App}/bin/cornix-build-v41";
            meta.description = "Build the unified v4.1 half-duplex firmware";
          };
          uf2-v40 = {
            type = "app";
            program = "${uf2V40App}/bin/cornix-uf2-v40";
            meta.description = "Build the unified v4.0 fallback firmware and UF2";
          };
          build-v40 = {
            type = "app";
            program = "${buildV40App}/bin/cornix-build-v40";
            meta.description = "Build the unified v4.0 fallback firmware";
          };
        };
        devShells.default =
          with pkgs;
          mkShell {
            name = "cornix";
            buildInputs = [
              rust
              probe-rs-tools
              picotool
              elf2uf2-rs
            ];
          };
      }
    );
}
