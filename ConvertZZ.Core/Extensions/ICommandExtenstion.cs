using System;
using System.Collections.Generic;
using System.Data;
using System.Linq;
using System.Text;
using System.Threading.Tasks;
using System.Windows.Input;
using CommunityToolkit.Mvvm.Input;

namespace ConvertZZ.Core.Extensions
{
    public static class ICommandExtenstion
    {
        public static ICommand Combine(this ICommand command1, ICommand command2, object? parameter1 = null, object? parameter2 = null)
        {
            return new RelayCommand(() =>
            {
                command1.Execute(parameter1);
                command2.Execute(parameter2);
            }, () => command1.CanExecute(parameter1) && command2.CanExecute(parameter2));
        }

        //public static ICommand Combine<T>(this ICommand command1, IRelayCommand<T> command2)
        //{
        //    return new RelayCommand(() =>
        //    {
        //        command1.Execute(null);
        //        command2.Execute(null);
        //    }, () => command1.CanExecute(null) && command2.CanExecute(null));
        //}
    }
}