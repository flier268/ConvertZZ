namespace ConvertZZ.Core.Test.Helpers
{
    [TestFixture]
    public class MessengerHelperTests
    {
        [Test]
        public void RegistInvokeCommandMessage_return_a_command_and_can_be_invoke()
        {
            int i = 0;
            var command = MessengerHelper.RegistInvokeCommandMessageAndReturnCommand(this, () =>
            {
                i = i * 2 + 1;
            }, default);
            Assert.Multiple(() =>
            {
                int anwser = 0;
                Assert.That(i, Is.EqualTo(anwser));
                for (int j = 0; j < 100; j++)
                {
                    anwser = anwser * 2 + 1;
                    command.Execute(null);
                    Assert.That(i, Is.EqualTo(anwser));
                }
            });
        }

        [Test]
        public void RegistInvokeCommandMessage_async_1_parameter_1_callback_regist_a_message()
        {
            WeakReferenceMessenger.Default.UnregisterAll(this);
            int i = 0;
            int anwser = 0;

            var commands = Enum.GetValues<EnumCommand>().ToList();
            Assert.Multiple(async () =>
            {
                foreach (var command in commands)
                {
                    MessengerHelper.RegistInvokeCommandMessage(this, async (int j) =>
                    {
                        j = j * 2 + 1;
                        return await Task.FromResult(j);
                    }, command);

                    var adapter = new EnumAdapter<EnumCommand>(command);

                    Assert.That(WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<int, int>, EnumAdapter<EnumCommand>>(this, adapter), Is.True);
                    int? invokeCommandMessage = await WeakReferenceMessenger.Default.Send(new AsyncInvokeCommandMessage<int, int>(i), adapter);
                    anwser = i * 2 + 1;
                    Assert.That(anwser, Is.EqualTo(invokeCommandMessage));
                    i = anwser;
                    if (commands.IndexOf(command) + 1 < commands.Count)
                    {
                        adapter = new EnumAdapter<EnumCommand>(commands[commands.IndexOf(command) + 1]);
                        Assert.That(WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<int, int>, EnumAdapter<EnumCommand>>(this, adapter), Is.False);
                    }
                }
            });
        }

        [Test]
        public void RegistInvokeCommandMessage_1_parameter_1_callback_regist_a_message()
        {
            WeakReferenceMessenger.Default.UnregisterAll(this);
            int i = 0;
            int anwser = 0;

            var commands = Enum.GetValues<EnumCommand>().ToList();
            Assert.Multiple(async () =>
            {
                foreach (var command in commands)
                {
                    MessengerHelper.RegistInvokeCommandMessage(this, (int j) =>
                    {
                        j = j * 2 + 1;
                        return j;
                    }, command);

                    var adapter = new EnumAdapter<EnumCommand>(command);

                    Assert.That(WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<int, int>, EnumAdapter<EnumCommand>>(this, adapter), Is.True);
                    int? invokeCommandMessage = await WeakReferenceMessenger.Default.Send(new AsyncInvokeCommandMessage<int, int>(i), adapter);
                    anwser = i * 2 + 1;
                    Assert.That(anwser, Is.EqualTo(invokeCommandMessage));
                    i = anwser;
                    if (commands.IndexOf(command) + 1 < commands.Count)
                    {
                        adapter = new EnumAdapter<EnumCommand>(commands[commands.IndexOf(command) + 1]);
                        Assert.That(WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<int, int>, EnumAdapter<EnumCommand>>(this, adapter), Is.False);
                    }
                }
            });
        }

        [Test]
        public void RegistInvokeCommandMessage_async_0_parameter_1_callback_regist_a_message()
        {
            WeakReferenceMessenger.Default.UnregisterAll(this);
            int i = 0;
            int anwser = 0;

            var commands = Enum.GetValues<EnumCommand>().ToList();
            Assert.Multiple(async () =>
            {
                foreach (var command in commands)
                {
                    MessengerHelper.RegistInvokeCommandMessage(this, async () =>
                    {
                        i = i * 2 + 1;
                        return await Task.FromResult(i);
                    }, command);

                    var adapter = new EnumAdapter<EnumCommand>(command);

                    Assert.That(WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<int>, EnumAdapter<EnumCommand>>(this, adapter), Is.True);
                    int? invokeCommandMessage = await WeakReferenceMessenger.Default.Send(new AsyncInvokeCommandMessage<int>(), adapter);
                    anwser = anwser * 2 + 1;
                    Assert.That(anwser, Is.EqualTo(invokeCommandMessage));
                    i = anwser;
                    if (commands.IndexOf(command) + 1 < commands.Count)
                    {
                        adapter = new EnumAdapter<EnumCommand>(commands[commands.IndexOf(command) + 1]);
                        Assert.That(WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<int>, EnumAdapter<EnumCommand>>(this, adapter), Is.False);
                    }
                }
            });
        }

        [Test]
        public void RegistInvokeCommandMessage_0_parameter_1_callback_regist_a_message()
        {
            WeakReferenceMessenger.Default.UnregisterAll(this);
            int i = 0;
            int anwser = 0;

            var commands = Enum.GetValues<EnumCommand>().ToList();
            Assert.Multiple(async () =>
            {
                foreach (var command in commands)
                {
                    MessengerHelper.RegistInvokeCommandMessage(this, () =>
                    {
                        i = i * 2 + 1;
                        return i;
                    }, command);

                    var adapter = new EnumAdapter<EnumCommand>(command);

                    Assert.That(WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<int>, EnumAdapter<EnumCommand>>(this, adapter), Is.True);
                    int? invokeCommandMessage = await WeakReferenceMessenger.Default.Send(new AsyncInvokeCommandMessage<int>(), adapter);
                    anwser = anwser * 2 + 1;
                    Assert.That(anwser, Is.EqualTo(invokeCommandMessage));
                    i = anwser;
                    if (commands.IndexOf(command) + 1 < commands.Count)
                    {
                        adapter = new EnumAdapter<EnumCommand>(commands[commands.IndexOf(command) + 1]);
                        Assert.That(WeakReferenceMessenger.Default.IsRegistered<AsyncInvokeCommandMessage<int>, EnumAdapter<EnumCommand>>(this, adapter), Is.False);
                    }
                }
            });
        }

        [Test]
        public void SendAsync_1_parameter_1_callback()
        {
            WeakReferenceMessenger.Default.UnregisterAll(this);
            int i = 0;

            var commands = Enum.GetValues<EnumCommand>().ToList();
            Assert.Multiple(async () =>
            {
                foreach (var command in commands)
                {
                    MessengerHelper.RegistInvokeCommandMessage(this, (int j) =>
                    {
                        j = j * 2 + 1;
                        return j;
                    }, command);

                    int h = await MessengerHelper.SendAsync<int, int>(command, i);
                    Assert.That(h, Is.EqualTo(i * 2 + 1));
                    i = h;
                }
            });
        }

        [Test]
        public void SendAsync_async_1_parameter_1_callback()
        {
            WeakReferenceMessenger.Default.UnregisterAll(this);
            int i = 0;

            var commands = Enum.GetValues<EnumCommand>().ToList();
            Assert.Multiple(async () =>
            {
                foreach (var command in commands)
                {
                    MessengerHelper.RegistInvokeCommandMessage(this, async (int j) =>
                    {
                        j = j * 2 + 1;
                        return await Task.FromResult(j);
                    }, command);

                    int h = await MessengerHelper.SendAsync<int, int>(command, i);
                    Assert.That(h, Is.EqualTo(i * 2 + 1));
                    i = h;
                }
            });
        }

        [Test]
        public void SendAsync_0_parameter_1_callback()
        {
            WeakReferenceMessenger.Default.UnregisterAll(this);
            int i = 0;

            var commands = Enum.GetValues<EnumCommand>().ToList();
            Assert.Multiple(async () =>
            {
                foreach (var command in commands)
                {
                    MessengerHelper.RegistInvokeCommandMessage(this, () =>
                    {
                        int j = i * 2 + 1;
                        return j;
                    }, command);

                    int h = await MessengerHelper.SendAsync<int>(command);
                    Assert.That(h, Is.EqualTo(i * 2 + 1));
                    i = h;
                }
            });
        }

        [Test]
        public void SendAsync_async_0_parameter_1_callback()
        {
            WeakReferenceMessenger.Default.UnregisterAll(this);
            int i = 0;

            var commands = Enum.GetValues<EnumCommand>().ToList();
            Assert.Multiple(async () =>
            {
                foreach (var command in commands)
                {
                    MessengerHelper.RegistInvokeCommandMessage(this, async () =>
                    {
                        int j = i * 2 + 1;
                        return await Task.FromResult(j);
                    }, command);

                    int h = await MessengerHelper.SendAsync<int>(command);
                    Assert.That(h, Is.EqualTo(i * 2 + 1));
                    i = h;
                }
            });
        }
    }
}