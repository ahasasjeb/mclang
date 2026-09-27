// Runtime acceptance tests: real Minecraft 26.3 classes and generated mcfunctions.
// This is not a compiler mock or a unit test. All gameplay actions click actual signs.
import com.mojang.authlib.GameProfile;
import io.netty.channel.embedded.EmbeddedChannel;
import java.util.*;
import net.minecraft.SharedConstants;
import net.minecraft.core.BlockPos;
import net.minecraft.core.registries.Registries;
import net.minecraft.gametest.framework.*;
import net.minecraft.network.Connection;
import net.minecraft.network.chat.Component;
import net.minecraft.network.protocol.PacketFlow;
import net.minecraft.resources.Identifier;
import net.minecraft.resources.ResourceKey;
import net.minecraft.server.MinecraftServer;
import net.minecraft.server.level.ServerLevel;
import net.minecraft.server.level.ServerPlayer;
import net.minecraft.server.network.CommonListenerCookie;
import net.minecraft.server.commands.data.EntityDataAccessor;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.level.block.Blocks;
import net.minecraft.world.level.block.entity.SignBlockEntity;
import net.minecraft.world.level.block.entity.SignText;
import net.minecraft.world.level.block.entity.SignTextSlot;
import net.minecraft.world.scores.ScoreHolder;

public class SignBankRuntime {
    static MinecraftServer server;
    static ServerLevel level;
    static int assertions;
    public static void main(String[] args) throws Exception {
        SharedConstants.tryDetectVersion();
        TestFunctionLoader.registerLoader(register -> register.accept(
            ResourceKey.create(Registries.TEST_FUNCTION, Identifier.parse("sign_bank_test:integration")), helper -> {
                helper.runAtTickTime(60, () -> {
                    try {
                        server = helper.getLevel().getServer(); level = server.overworld();
                        verify();
                        System.out.println("SIGN_BANK_RUNTIME_PASS assertions=" + assertions);
                        helper.succeed();
                    } catch (Throwable failure) {
                        failure.printStackTrace(); helper.fail(Component.literal(failure.toString()));
                    }
                });
            }));
        GameTestMainUtil.runGameTestServer(args, unused -> {});
    }
    static void check(boolean success, String label) {
        assertions++;
        if (!success) throw new AssertionError(label);
        System.out.println("PASS " + label);
    }
    static void command(String text) {
        server.getCommands().performPrefixedCommand(server.createCommandSourceStack().withLevel(level), text);
    }
    static void button(ServerPlayer player, String function) {
        BlockPos pos;
        if (function.startsWith("auth/")) {
            pos = new BlockPos(function.equals("auth/button_register") || function.equals("auth/button_home") ? 1 : -1, 65, 1);
        } else {
            int base = 100 + 16 * (score(player, "session") - 1);
            pos = switch (function) {
                case "bank/deposit" -> new BlockPos(base + 1, 66, 1);
                case "bank/withdraw" -> new BlockPos(base + 3, 66, 1);
                case "bank/upgrade" -> new BlockPos(base + 1, 65, 1);
                case "bank/logout" -> new BlockPos(base + 3, 65, 1);
                default -> throw new AssertionError(function);
            };
        }
        var sign = (SignBlockEntity) level.getBlockEntity(pos);
        if (sign == null || !sign.canExecuteClickCommands(SignTextSlot.FRONT, player)) throw new AssertionError("Missing click command: " + function);
        sign.executeClickCommandsIfPresent(level, player, pos, SignTextSlot.FRONT);
    }
    static int score(ScoreHolder holder, String name) {
        var board = server.getScoreboard(); var objective = board.getObjective("sign_bank_" + name);
        if (objective == null) throw new AssertionError("Missing objective " + name);
        var value = board.getPlayerScoreInfo(holder, objective);
        return value == null ? 0 : value.value();
    }
    static List<Entity> tagged(String tag) {
        List<Entity> entities = new ArrayList<>();
        for (Entity entity : level.getAllEntities()) if (entity.entityTags().contains(tag) && !entity.isRemoved()) entities.add(entity);
        return entities;
    }
    static Entity record(int slot) {
        return tagged("sb_account").stream().filter(e -> score(e, "account_slot") == slot).findFirst().orElseThrow();
    }
    static ServerPlayer player(String name) {
        var cookie = CommonListenerCookie.createInitial(new GameProfile(UUID.randomUUID(), name), false);
        var player = new ServerPlayer(server, level, cookie.gameProfile(), cookie.clientInformation()) {
            @Override public boolean isClientAuthoritative() { return false; }
        };
        var connection = new Connection(PacketFlow.SERVERBOUND); new EmbeddedChannel(connection);
        server.getPlayerList().placeNewPlayer(connection, player, cookie); player.setPos(0.5, 64, 5.5);
        return player;
    }
    static void input(String text) {
        var sign = (SignBlockEntity) level.getBlockEntity(new BlockPos(0, 66, 1));
        if (sign == null) throw new AssertionError("Missing input sign");
        var value = SignText.EMPTY.asMutable(); value.setLine(0, Component.literal(text));
        sign.setText(value.asImmutable(), SignTextSlot.FRONT);
    }
    static String line(int slot, boolean password) {
        var pos = new BlockPos(-4 + (slot - 1) % 8, 66 + (password ? 1 : 0) + 3 * ((slot - 1) / 8), -7);
        var sign = (SignBlockEntity) level.getBlockEntity(pos);
        return sign == null ? "<missing>" : sign.getText(SignTextSlot.FRONT).getMessages(false).getFirst().getString();
    }
    static void logout(ServerPlayer player) { button(player, "bank/logout"); }
    static void register(ServerPlayer p, String name, String password, int slot) {
        p.setPos(0.5,64,5.5); button(p,"auth/button_register");
        check(score(p,"auth_mode")==3,"register page "+slot);
        input(name); button(p,"auth/register_account");
        check(score(p,"auth_mode")==4,"account accepted "+name);
        check(score(p,"pending_slot")==slot,"reservation slot "+slot);
        check(line(slot,false).equals(name),"account readback "+name);
        input(password); button(p,"auth/register_password");
        check(score(p,"session")==slot,"registered session "+name);
        check(line(slot,true).equals(password),"separate password readback "+slot);
        check(tagged("sb_display").size()==1,"one display "+slot);
        logout(p); check(tagged("sb_display").isEmpty(),"logout cleanup "+slot);
    }
    static void login(ServerPlayer p,String name,String password,int slot) {
        p.setPos(0.5,64,5.5);
        if (score(p,"auth_mode")!=0) button(p,"auth/button_home");
        command("function sign_bank:ui/show_home_menu");
        button(p,"auth/button_login"); input(name); button(p,"auth/login_account");
        check(score(p,"auth_mode")==2,"login found "+name);
        check(score(p,"pending_slot")==slot,"correct login slot "+slot);
        input(password); button(p,"auth/login_password");
        check(score(p,"session")==slot,"password accepted "+slot);
    }
    static void verify() {
        check(tagged("sb_account").size()==32,"32 unique markers after automatic installation");
        var a=player("BankTesterA"); command("give BankTesterA minecraft:emerald 64");
        String[] names={"123","00123","lzy","中文账户","a\"b\\c","true","12abc","lzy2"};
        String[] passwords={"abc","中文密码","pw123","a\"b\\c","false","00123","-0.5","pw123"};
        for(int i=0;i<names.length;i++) register(a,names[i],passwords[i],i+1);
        for(int i=0;i<names.length;i++) {
            login(a,names[i],passwords[i],i+1);
            check(a.getX()>=100+16*i && a.getX()<112+16*i,"correct physical room "+(i+1)); logout(a);
        }
        button(a,"auth/button_register"); input("lzy"); button(a,"auth/register_account");
        check(score(a,"auth_mode")==3,"duplicate name rejected"); button(a,"auth/button_home");
        button(a,"auth/button_login"); input("lzy"); button(a,"auth/login_account"); input("abc"); button(a,"auth/login_password");
        check(score(a,"session")==0,"other account password rejected");
        login(a,"lzy","pw123",3); button(a,"bank/deposit");
        check(score(record(3),"account_balance")==56,"registration cost and deposit");
        check(score(record(1),"account_balance")==0,"balance isolation");
        check(new EntityDataAccessor(tagged("sb_display").getFirst()).getData().get("text").toString().contains("56"),"display resolved balance");
        button(a,"bank/withdraw"); check(score(record(3),"account_balance")==55,"withdraw exactly one");
        int base=132;
        command("setblock 133 64 3 minecraft:chest{Items:[{Slot:0b,id:\"minecraft:diamond\",count:7}]}");
        var chest=level.getBlockEntity(new BlockPos(133,64,3)).saveWithoutMetadata(level.registryAccess());
        check(level.getBlockState(new BlockPos(base+4,64,3)).is(Blocks.BEDROCK),"initial visible 3x3 boundary");
        for(int width=4;width<=10;width++) {
            button(a,"bank/upgrade"); check(score(record(3),"account_size")==width,"upgrade size "+width);
            check(level.getBlockState(new BlockPos(base+width+1,64,3)).is(Blocks.BEDROCK),"bedrock outer wall "+width);
            check(level.getBlockState(new BlockPos(base+width,69,width+2)).is(Blocks.LIGHT),"light ceiling "+width);
            check(level.getBlockEntity(new BlockPos(133,64,3)).saveWithoutMetadata(level.registryAccess()).equals(chest),"chest preserved "+width);
        }
        check(score(record(3),"account_balance")==6,"upgrade costs total 49");
        check(score(record(2),"account_size")==3,"room isolation");
        button(a,"bank/upgrade"); check(score(record(3),"account_balance")==6,"max size no charge");
        check(tagged("sb_display").size()==1,"no duplicate display"); logout(a);
        var b=player("BankTesterB"); button(a,"auth/button_login");
        check(score(a,"auth_mode")==0,"two terminal players rejected");
        b.setPos(50,64,50); login(a,"lzy","pw123",3); b.setPos(a.getX()+0.5,64,a.getZ());
        button(a,"bank/withdraw"); check(score(record(3),"account_balance")==6,"intruder blocks transaction");
        command("function sign_bank:tick");
        check(score(a,"session")==0 && a.getX()<10 && b.getX()<10,"both occupants evicted");
        check(tagged("sb_display").isEmpty(),"forced logout cleanup");
        b.setPos(50,64,50); login(a,"lzy","pw123",3); command("function sign_bank:load");
        check(tagged("sb_display").isEmpty() && score(a,"session")==0,"reload clears sessions and display");
        check(score(record(3),"account_balance")==6 && line(3,false).equals("lzy"),"reload keeps data");
        check(level.getBlockEntity(new BlockPos(133,64,3)).saveWithoutMetadata(level.registryAccess()).equals(chest),"reload keeps chest");
        command("give BankTesterA minecraft:emerald 64");
        button(a,"auth/button_register"); input("cancelled"); button(a,"auth/register_account");
        check(score(a,"pending_slot")==9,"reserve next free slot");
        input(""); button(a,"auth/register_password");
        check(score(a,"auth_mode")==4 && score(a,"session")==0,"empty password does not complete registration");
        button(a,"auth/button_home");
        check(line(9,false).equals("<missing>") && line(9,true).equals("<missing>"),"cancel releases both signs");
        for(int slot=9;slot<=32;slot++) register(a,"slot"+slot,"secret"+slot,slot);
        login(a,"slot32","secret32",32); logout(a);
        button(a,"auth/button_register"); input("overflow"); button(a,"auth/register_account");
        check(score(a,"auth_mode")==3 && score(a,"pending_slot")==0,"33rd account refused");
        check(line(1,false).equals("123") && line(32,false).equals("slot32"),"full bank did not overwrite any slot");
        button(a,"auth/button_home");
        command("summon minecraft:marker 138 67 7 {Tags:[\"sb_account\",\"probe_duplicate\"]}");
        command("scoreboard players set @e[tag=probe_duplicate] sign_bank_account_slot 3");
        command("scoreboard players set @e[tag=probe_duplicate] sign_bank_account_ready 0");
        button(a,"auth/button_login"); input("lzy"); button(a,"auth/login_account");
        check(score(a,"auth_mode")==1 && score(a,"session")==0,"duplicate slot fails closed");
        command("function sign_bank:accounts/clear_uncommitted");
        check(line(3,false).equals("lzy") && line(3,true).equals("pw123"),"corruption cleanup preserves completed credentials");
        command("kill @e[tag=probe_duplicate]");
        command("clone -3 66 -7 -3 66 -7 9 68 -7 replace force");
        command("clone -4 66 -7 -4 66 -7 -3 66 -7 replace force");
        button(a,"auth/button_home"); button(a,"auth/button_login"); input("123"); button(a,"auth/login_account");
        check(score(a,"auth_mode")==1 && score(a,"pending_slot")==0,"duplicate account text rejected without arbitrary selection");
        command("clone 9 68 -7 9 68 -7 -3 66 -7 replace force");
        // Adjacent independent sessions remain valid.
        button(a,"auth/button_home"); login(a,"123","abc",1);
        b.setPos(0.5,64,5.5); login(b,"00123","中文密码",2);
        command("function sign_bank:tick");
        check(score(a,"session")==1 && score(b,"session")==2,"neighboring rooms have independent privacy checks");
        logout(a); logout(b);
        check(tagged("sb_display").isEmpty(),"all display entities cleaned");
        b.setPos(50,64,50); login(a,"lzy","pw123",3);
        server.getPlayerList().remove(a);
        command("function sign_bank:displays/cleanup_displays");
        check(tagged("sb_display").isEmpty(),"disconnect cleans orphan display");
        check(score(record(3),"account_balance")==6,"disconnect preserves balance");
    }
}
