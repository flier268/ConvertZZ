using System.Net;
using System.Net.Sockets;

namespace ConvertZZ.Core.Test.Helpers
{
    [TestFixture]
    public class TcpIpHelperTests
    {
        [Test]
        public void GetAllProcesses_StateUnderTest_ExpectedBehavior()
        {
            var listener = new TcpListener(IPAddress.Any, 0);
            listener.Start();
            var port = ((IPEndPoint)listener.LocalEndpoint).Port;

            // Act
            var result = TcpIpHelper.GetAllProcesses();
            listener.Stop();
            Assert.True(result.Any(x => x.Port == port));
        }

        [Test]
        public void GetRandomUnusedPort_StateUnderTest_ExpectedBehavior()
        {
            // Act
            var result = TcpIpHelper.GetRandomUnusedPort();

            Assert.DoesNotThrow(() =>
            {
                var listener = new TcpListener(IPAddress.Any, result);
                listener.Start();
                listener.Stop();
            });
        }
    }
}