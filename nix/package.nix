{ lib
, appimageTools
, fetchurl
, makeDesktopItem
, makeWrapper
}:

let
  pname = "csclient";
  version = "2.12.0";

  src = fetchurl {
    url = "https://github.com/baksyaaaaa/SCClient/releases/download/csclient-${version}/CSClient_${version}_linux_x86_64.AppImage";
    hash = "sha256-XNvFLNG2enLgjj7KSQAax+GFsnZXci/Chr2lOmbVz2M=";
  };

  extracted = appimageTools.extract { inherit pname version src; };

  desktopItem = makeDesktopItem {
    name = pname;
    desktopName = "CSClient";
    comment = "Minecraft launcher by CS";
    exec = "${pname} %U";
    icon = pname;
    terminal = false;
    categories = [ "Game" ];
    mimeTypes = [ "x-scheme-handler/csclient" ];
  };
in
appimageTools.wrapType2 {
  inherit pname version src;

  nativeBuildInputs = [ makeWrapper ];

  extraPkgs = pkgs: with pkgs; [
    libgpg-error
    libgcrypt
    vulkan-loader
    libxkbcommon
    wayland
    jdk8
    jdk17
    jdk21
    libGL
    openal
    libpulseaudio
    libxcursor
    libxrandr
    libxxf86vm
  ];

  extraInstallCommands = ''
    wrapProgram $out/bin/${pname} --unset GIO_EXTRA_MODULES

    install -Dm444 ${desktopItem}/share/applications/*.desktop \
      -t $out/share/applications

    find ${extracted} -maxdepth 2 -name '*.png' | head -n1 | while read -r icon; do
      install -Dm444 "$icon" $out/share/pixmaps/${pname}.png
    done
  '';

  meta = {
    description = "CSClient, Minecraft launcher by CS";
    homepage = "https://github.com/baksyaaaaa/SCClient";
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = [ "x86_64-linux" ];
    mainProgram = pname;
  };
}
