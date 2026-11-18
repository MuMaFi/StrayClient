package cc.straysmp.capes.mixin;

import cc.straysmp.capes.menu.ClientConfig;
import cc.straysmp.capes.menu.StrayTitleScreen;
import cc.straysmp.capes.menu.Tr;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.Tooltip;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.gui.screens.TitleScreen;
import net.minecraft.network.chat.Component;
import org.spongepowered.asm.mixin.Mixin;
import org.spongepowered.asm.mixin.injection.At;
import org.spongepowered.asm.mixin.injection.Inject;
import org.spongepowered.asm.mixin.injection.callback.CallbackInfo;

/** Minecraft's own title screen gets a button back to the Stray main menu. */
@Mixin(TitleScreen.class)
public abstract class TitleScreenMixin extends Screen {
    protected TitleScreenMixin(Component title) {
        super(title);
    }

    @Inject(method = "init", at = @At("TAIL"), require = 0)
    private void strayclient$backToStray(CallbackInfo ci) {
        addRenderableWidget(Button.builder(Tr.t("strayclient.title.backStray"), b -> {
            ClientConfig.setCustomMenu(true);
            minecraft.gui.setScreen(new StrayTitleScreen());
        }).bounds(6, 6, 116, 20).tooltip(Tooltip.create(Tr.t("strayclient.title.backStray.tip"))).build());
    }
}
