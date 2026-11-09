package cc.straysmp.capes.mixin;

import cc.straysmp.capes.menu.StrayMenuScreen;
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyboardHandler;
import net.minecraft.client.Minecraft;
import net.minecraft.client.input.KeyEvent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Right Shift in game (no screen open) opens the Stray Client menu. PvP Plus uses the same key
 * for its own menu; ours takes it and links to PvP Plus' settings instead.
 */
@Mixin(KeyboardHandler.class)
public abstract class KeyboardHandlerMixin {
    @Inject(method = "keyPress", at = @At("HEAD"), cancellable = true)
    private void strayclient$menuKey(long window, int action, KeyEvent event, CallbackInfo ci) {
        if (action != InputConstants.PRESS || event.key() != InputConstants.KEY_RSHIFT) return;
        Minecraft mc = Minecraft.getInstance();
        if (mc.gui.screen() != null || mc.player == null) return;
        mc.gui.setScreen(new StrayMenuScreen());
        ci.cancel();
    }
}
