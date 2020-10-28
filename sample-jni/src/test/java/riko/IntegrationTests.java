package riko;

import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.nio.file.Paths;
import org.bson.BsonBinary;
import org.bson.BsonBoolean;
import org.bson.BsonDocument;
import org.bson.BsonInt32;
import org.bson.BsonString;
import org.bson.BsonValue;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;

class IntegrationTests {
  static {
    final String library =
        Paths.get("..", "target", "debug", "libriko_sample.so").toAbsolutePath().toString();
    System.load(library);
  }

  @Test
  void nothing() {
    Assertions.assertDoesNotThrow(riko_sample.Module::nothing);
  }

  @Test
  void i32() {
    Assertions.assertEquals(
        2,
        riko_sample.Module._i32(new BsonInt32(1), new BsonInt32(1)).asInt32().intValue()
    );
  }

  @Test
  void rename() {
    Assertions.assertDoesNotThrow(riko_sample.Module::rename);
  }

  @Test
  void result_option() {
    Assertions.assertEquals(
        2,
        riko_sample.Module.result_option(new BsonInt32(1), new BsonInt32(1)).asInt32().intValue()
    );
    Assertions.assertThrows(
        ReturnedException.class, () -> riko_sample.Module.result_option(null, null));
    Assertions.assertTrue(riko_sample.Module.result_option(null, new BsonInt32(1)).isNull());
  }

  @Test
  void marshal() {
    Assertions.assertEquals(-1, riko_sample.Module.marshal(new BsonInt32(1)).asInt32().intValue());
  }

  @Test
  void string() {
    Assertions.assertEquals(
        "love you",
        riko_sample.Module.string(
            new BsonString("love"),
            new BsonString(" you")
        ).asString().getValue()
    );
  }

  @Test
  void bytes() {
    byte[] a = {1, 2, 3};
    byte[] b = {4, 5, 6};
    byte[] expected = {1, 2, 3, 4, 5, 6};
    Assertions.assertArrayEquals(
        expected,
        riko_sample.Module.bytes(new BsonBinary(a), new BsonBinary(b)).asBinary().getData()
    );
  }

  @Test
  void bool() {
    assertFalse(riko_sample.Module._bool(
        new BsonBoolean(false),
        new BsonBoolean(true)
    ).asBoolean().getValue());
  }

  @Test
  void structs() {
    final BsonValue love = new BsonDocument("target", new BsonString("Lan"));
    final BsonValue work = new BsonDocument("salary", new BsonInt32(10000));
    final BsonValue life = riko_sample.structs.Module.structs(love, work);
    assertTrue(life.asDocument().getBoolean("happy").getValue());
  }

  @Test
  void object() {
    final riko.Object object = riko_sample.object.Module.create_reactor();
    assertTrue(object.alive());
    object.close();
    assertFalse(object.alive());
  }
}
