package cc.straysmp.capes.mixin;

import cc.straysmp.capes.menu.StrayMenuScreen;
import cc.straysmp.capes.menu.StrayTitleScreen;
import com.mojang.blaze3d.platform.InputConstants;
import net.minecraft.client.KeyboardHandler;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.client.input.KeyEvent;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/**
 * Right Shift in game (no screen open) or on the main menu opens the Stray Client menu. PvP Plus uses the same key
 * for its own menu; ours takes it and links to PvP Plus' settings instead.
 */
@Mixin(KeyboardHandler.class)
public abstract class KeyboardHandlerMixin {
    @Inject(method = "keyPress", at = @At("HEAD"), cancellable = true)
    private void strayclient$menuKey(long window, int action, KeyEvent event, CallbackInfo ci) {
        if (action != InputConstants.PRESS || event.key() != InputConstants.KEY_RSHIFT) return;
        Minecraft mc = Minecraft.getInstance();
        Screen screen = mc.gui.screen();
        boolean inGame = screen == null && mc.player != null;
        boolean mainMenu = screen instanceof TitleScreen || screen instanceof StrayTitleScreen;
        if (!inGame && !mainMenu) return;
        mc.gui.setScreen(new StrayMenuScreen(screen));
        ci.cancel();
    }
}
