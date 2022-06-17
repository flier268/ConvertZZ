using System.Windows.Input;
using CommunityToolkit.Mvvm.Input;
using CommunityToolkit.Mvvm.Messaging;
using ConvertZZ.Core.Messages;

namespace ConvertZZ.Core.Helpers
{
    public partial class MessengerHelper
    {
        public static ICommand RegistInvokeCommandMessage<TRecipient>(TRecipient recipient, Action action, EnumCommand enumCommand) where TRecipient : class
        {
            RelayCommand relayCommand = new(action);
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<InvokeCommandMessage, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<InvokeCommandMessage, EnumAdapter<EnumCommand>>(recipient, adapter, (r, m) =>
                {
                    relayCommand.Execute(null);
                    m.Reply(-1);
                });
            }
            return relayCommand;
        }

        public static void RegistInvokeCommandMessage<TRecipient, T>(TRecipient recipient, Func<T> action, EnumCommand enumCommand) where TRecipient : class
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<InvokeCommandMessage<T>, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<InvokeCommandMessage<T>, EnumAdapter<EnumCommand>>(recipient, adapter, (r, m) =>
                {
                    T timeCost = action();
                    m.Reply(timeCost);
                });
            }
            return;
        }

        public static void RegistInvokeCommandMessage<TRecipient, T>(TRecipient recipient, Func<Task<T>> action, EnumCommand enumCommand) where TRecipient : class
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<InvokeCommandMessage<T>, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<InvokeCommandMessage<T>, EnumAdapter<EnumCommand>>(recipient, adapter, async (r, m) =>
                {
                    m.Reply(Task.Run(action));
                });
            }
            return;
        }

        public static void RegistInvokeCommandMessage<TRecipient, TParameter, T>(TRecipient recipient, Func<TParameter?, T> action, EnumCommand enumCommand) where TRecipient : class
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<InvokeCommandMessage<T>, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<InvokeCommandMessage<T>, EnumAdapter<EnumCommand>>(recipient, adapter, (r, m) =>
                {
                    T timeCost = action((TParameter?)m.Parameter);
                    m.Reply(timeCost);
                });
            }
            return;
        }

        //public static ICommand Regist<TRecipient>(TRecipient recipient, Func<long> action, EnumCommand enumCommand) where TRecipient : class
        //{
        //    //RelayCommand relayCommand = new(() => action());

        //    RelayCommand relayCommand = new(() =>
        //    {
        //        WeakReferenceMessenger.Default.Send(new InvokeCommandMessage(), new EnumAdapter<EnumCommand>(enumCommand));
        //    });

        //    var adapter = new EnumAdapter<EnumCommand>(enumCommand);

        //    if (WeakReferenceMessenger.Default.IsRegistered<InvokeCommandMessage, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
        //    {
        //        WeakReferenceMessenger.Default.Register<InvokeCommandMessage, EnumAdapter<EnumCommand>>(recipient, adapter, (r, m) =>
        //        {
        //            long timeCost = action();
        //            m.Reply(timeCost);
        //        });
        //    }
        //    return relayCommand;
        //}

        public static ICommand Regist<TRecipient, T>(TRecipient recipient, Action<T?> action, EnumCommand enumCommand) where TRecipient : class
        {
            RelayCommand<T> relayCommand = new(action);
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);

            if (WeakReferenceMessenger.Default.IsRegistered<InvokeCommandMessage, EnumAdapter<EnumCommand>>(recipient, adapter) == false)
            {
                WeakReferenceMessenger.Default.Register<InvokeCommandMessage, EnumAdapter<EnumCommand>>(recipient, adapter, (r, m) =>
                {
                    relayCommand.Execute(m.Parameter);
                    m.Reply(-1);
                });
            }
            return relayCommand;
        }

        public static void Send(EnumCommand enumCommand, object? parameter = null)
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);
            WeakReferenceMessenger.Default.Send(new InvokeCommandMessage(parameter), adapter);
        }

        public static T Send<T>(EnumCommand enumCommand, object? parameter = null)
        {
            return SendAsync<T>(enumCommand, parameter).Result;
        }

        public static async Task<T> SendAsync<T>(EnumCommand enumCommand, object? parameter = null)
        {
            var adapter = new EnumAdapter<EnumCommand>(enumCommand);
            return await WeakReferenceMessenger.Default.Send(new InvokeCommandMessage<T>(parameter), adapter);
        }
    }
}