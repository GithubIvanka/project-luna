# `luna-files`

**Status:** GUI boundary implemented; filesystem integration incomplete

## Purpose
Provide Luna's graphical file-manager client and present the user-facing DATA/volume model.

## Owns
- GTK4 file-manager UI;
- navigation/presentation;
- file/volume presentation;
- invoking backend file operations through the approved filesystem/task model.

## Does not own
Raw filesystem policy, device discovery, authorization, Bundle management or namespace enforcement.

## File model
The UI uses ordinary icon categories and readable metadata. Folders, images, audio, video, archives, PDF and source/configuration files have semantic presentation; the fallback is generic file presentation.

The home/up controls are UI affordances, not filesystem policy.

## Yazi relationship
Yazi 26.9.1 is packaged as the initial filesystem engine/tooling boundary. Packaging Yazi and configuring `backend_model=yazi-core` does **not** mean the current GUI has a direct `yazi-core` library integration. That integration remains future work.

## External volumes
The file manager receives friendly volume representations from `luna-device-manager`; it must not require users to work with `/dev/sdX` or manual mount commands.

## Dependencies
GTK4, Luna filesystem/task contracts, device/volume contracts and security-aware file access.

## Open
Same-window directory navigation, complete file operations, volume events and direct shared backend integration remain.
