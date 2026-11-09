package cc.straysmp.capes.mixin;

import net.minecraft.client.gui.screens.LoadingOverlay;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfoReturnable;

/**
 * Loading screen background: StraySMP navy (#1A1B24) instead of Mojang red.
 * BRAND_BACKGROUND is an IntSupplier lambda; require = 0 keeps the game starting even if a
 * future Minecraft version renames it (then the vanilla colour is used).
 */
@Mixin(LoadingOverlay.class)
public abstract class LoadingOverlayMixin {
    @Inject(method = "lambda$static$0", at = @At("HEAD"), cancellable = true, require = 0)
    private static void strayclient$background(CallbackInfoReturnable<Integer> cir) {
        cir.setReturnValue(0xFF1A1B24);
    }
}
