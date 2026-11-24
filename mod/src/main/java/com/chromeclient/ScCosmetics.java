package com.chromeclient;

import net.fabricmc.api.ClientModInitializer;

/**
 * SC Client cosmetics: draws the hats, capes, wings and cape-patterned elytras picked in the SC Client Wardrobe,
 * on you and on other SC Client players. Nothing else: no menu, HUD or music.
 * The class names stay in com.chromeclient because the SC menu mod (ChromePoly) looks them up by name.
 */
public final class ScCosmetics implements ClientModInitializer {
    public static final String MOD_ID = "sccosmetics";

    @Override
    public void onInitializeClient() {
        Cosmetics.load();
        Cosmetics.init();
        CosmeticSync.init();
    }
}
