import java.nio.file.Paths;
import org.junit.jupiter.api.Assertions;
import org.junit.jupiter.api.Test;
import riko.Any;

class IntegrationTests {
  static {
    final String library = Paths
        .get("..", "target", "debug", "libriko_sample.so")
        .toAbsolutePath()
        .toString();
    System.load(library);
  }

  @Test
  void nothing() {
    riko_sample.Module.nothing();
  }

  @Test
  void i32() {
    Assertions.assertEquals(2, riko_sample.Module._i32(1, 1));
  }

  @Test
  void rename() {
    riko_sample.Module.rename();
  }

  @Test
  void result_option() {
    Assertions.assertEquals(2, riko_sample.Module.result_option(1, 1));
    Assertions.assertThrows(Exception.class, () -> riko_sample.Module.result_option(null, null));
    Assertions.assertNull(riko_sample.Module.result_option(null, 1));
  }

  @Test
  void marshal() {
    Assertions.assertEquals(-1, riko_sample.Module.marshal(1));
  }

  @Test
  void string() {
    Assertions.assertEquals("love you", riko_sample.Module.string("love", " you"));
  }

  @Test
  void bytes() {
    byte[] a = { 1, 2, 3 };
    byte[] b = { 4, 5, 6 };
    byte[] expected = { 1, 2, 3, 4, 5, 6 };
    Assertions.assertArrayEquals(expected, riko_sample.Module.bytes(a, b));
  }

  @Test
  void bool() {
    Assertions.assertEquals(false, riko_sample.Module._bool(false, true));
  }

  @Test
  void any() {
    final Any result = riko_sample.object.Module.create_reactor();
    Assertions.assertTrue(result.alive());
    result.close();
    Assertions.assertFalse(result.alive());
  }
}